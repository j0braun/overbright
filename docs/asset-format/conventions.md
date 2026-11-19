# Asset format: common conventions

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md). Every other page of the format relies on
the rules here and does not repeat them.

## 1. Conformance language

The words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are used as in RFC 2119 / RFC 8174
(https://www.rfc-editor.org/rfc/rfc8174): only when in capitals. A *reader* is any program that loads files of the
format (the game, the launcher, Studio, the CLI, the web build); a *writer* is any program that creates or changes
them (the importer, Studio, the CLI, a pack author's own tools). A writer MUST produce files a conforming reader
accepts; a reader MUST refuse what this specification says it refuses and MUST NOT guess.

Tags used on every page, as in `docs/formats.md`: **verified** (traced in the original executable and checked),
**inferred** (believed, not traced), **open** (unknown). A field whose meaning is inferred or open is still carried
losslessly (section 8); only its *name* and *description* carry the tag.

## 2. Terms

| Term | Meaning |
|---|---|
| **pack** | A named, versioned set of asset files with one manifest (`pack.toml`, [manifest.md](manifest.md)). |
| **base set** | A pack with `kind = "base"`: the complete content the engine runs on. Made by the importer from the player's own game folder, or authored by a community ([layout.md](layout.md)). Exactly one base set is mounted at a time. |
| **mod pack** | A pack with `kind = "mod"`, mounted above the base set. |
| **layer stack** | The base set plus the enabled mod packs in order ([layering.md](layering.md)). |
| **logical path** | A file's name inside a pack (`world/L4RA/sections/1427.toml`), independent of where the pack's files are stored. |
| **stream section** | A region's unit of loading: one record of the region's section table (chunk 0x34110), with its number (`1427`), its range in the region's `STREAM` file and, in the format, its file `world/<REGION>/sections/<number>.toml` ([world.md](world.md)). Format-1 map mods' `section` key is a stream section. |
| **scenery section** | A numbered group of scenery placements (chunk 0x80034100). One stream section holds one or more (stream section 1427 of L4RA holds scenery sections 1427 and 1407); the original's links and visible lists name scenery sections. Placements are keyed by scenery section. The word *section* alone is not used in the format's normative text. |
| **bundle** | A ZIP file holding part of a pack's logical tree (`.obz`; a whole pack in one bundle is a `.obpk`) ([layout.md](layout.md) §4). |
| **source form** | The files this specification defines: open formats (TOML, glTF 2.0, KTX 2.0, PNG, FLAC, …) plus a few documented binary arrays. What packs contain, what tools edit, what the importer writes. Normative. |
| **cooked form** | What a reader derives from source files for fast loading (the asset cache). Documented in [cooked.md](cooked.md); never shared, always rebuildable from source. |
| **item** | One addressable thing inside a file kind: a texture, a solid, a placement, a car record, a string. Every item has a **key** (section 6). |
| **kind** | A family of items with one schema (`ob.texture-set`, `ob.section`, …). Each kind family has its own page. |
| **schema id** | A kind's name and version, `ob.<kind>/<major>.<minor>` (section 3.3, [versioning.md](versioning.md)). |
| **residual** | Original bytes the importer kept because no decoder (or no proven lossless decoder) exists for them yet ([residual.md](residual.md)). |
| **equality level** | The level at which a kind's imported data is proven equal to the original: bytes, decoded values, or behaviour ([parity.md](parity.md)). |

## 3. Text files

### 3.1 Encoding

- UTF-8 without a byte-order mark. A reader MUST refuse a BOM, invalid UTF-8 and NUL characters.
- Lines end in LF. A reader MUST also accept CRLF (files edited on Windows); writers MUST write LF.
- A file ends with a line feed. Readers MUST NOT require it.

### 3.2 TOML

- Overbright's own text kinds are **TOML 1.0** (https://toml.io/en/v1.0.0), extension `.toml`. glTF files are JSON
  as the glTF specification requires. No other text syntax is used for data.
- **Field names** are `snake_case` ASCII (`[a-z][a-z0-9_]*`). Tables whose keys are *data* (pack ids in `[dependencies]`,
  bundle file names, part names, indices written as strings) are typed `map<K,V>`: the field table gives the key
  pattern and the value type. A field's meaning, type, unit, default and limits are given by the
  kind's field table. A reader MUST refuse a key that the schema does not define, except inside `[extras]` and
  `[ext.<NAME>]` tables (section 9). Refusing unknown keys is deliberate: a misspelt key must never be a silent
  no-op.
- Field tables use these types:

  | Type | TOML form | Notes |
  |---|---|---|
  | `bool` | `true` / `false` | |
  | `u8` … `u64`, `i8` … `i64` | integer | MUST be within the type's range; a reader refuses others. TOML integers are signed 64-bit, so a `u64` field holds at most 2⁶³−1 in TOML; a kind that needs the full range uses two `u32` fields or a `bytes` value. |
  | `f32`, `f64` | float (or integer, read as that float) | Exactness rules in 3.4. |
  | `string` | basic or literal string | Length limits are in characters (Unicode scalar values) unless stated. |
  | `name` | string, 1–63 printable ASCII characters (0x20–0x7E), no leading or trailing space | A game-style name (section 6.3). Case is significant. Kind pages narrow it where the original does (car names: `[A-Z0-9_]`). |
  | `hash32` | string `"0x"` + exactly 8 upper-case hex digits | A 32-bit key (section 6.4). |
  | `digest` | string `"<alg>:<hex>"` | Section 7. |
  | `ref<kind>` | string `"<kind>:<key>"` | A reference to an item (section 6.6). |
  | `vec2`, `vec3`, `vec4` | array of 2/3/4 floats | Components in x, y, z, w order. |
  | `mat3` | array of 9 floats | Row-major, rows are the local axes (section 5.3). |
  | `color` | string `"#RRGGBBAA"` (upper-case hex) | sRGB-encoded 8-bit channels unless the field says linear. |
  | `enum` | string from a listed set | Lower-case `snake_case` values. |
  | `array<T>` | array | Bounds given per field. |
  | `map<K,V>` | table | Keys are data matching the pattern `K` the field table gives; values of type `V`. |
  | `table` | table / inline table | Fields given by a sub-table in the page. |
  | `bytes` | string `"hex:"` + even number of lower-case hex digits | Short opaque data only (≤ 4096 bytes); longer data goes to a binary file (4.4). |
  | `date` | string `"2026-11-18T12:00:00Z"` | RFC 3339, UTC. A string, not a TOML date-time value, so every TOML-to-JSON mapping and schema validator treats it alike. |

- Text whose exact characters matter (strings, credits, legal texts) is written as basic strings with escapes
  (`\n`, `\t`, `\u0085`), never as multi-line strings: TOML parsers may normalise line endings inside multi-line
  strings, and the format keeps every character as the original has it.
- Integers that are bit sets or ids MAY be written as hex integers (`0x0200`); readers MUST accept decimal, hex
  (`0x`), octal and binary TOML integers alike. Writers write flags and ids in hex, counts in decimal.

### 3.3 The `schema` key

Every TOML data file of the format starts with a top-level string `schema = "ob.<kind>/<major>.<minor>"`, naming
the kind and the **lowest** version of that kind that can read the file ([versioning.md](versioning.md) §2.1). A
reader MUST refuse a file without it, of an unknown kind, of a major version it does not read, or of a higher
minor version than it knows. `pack.toml` and `order.txt` are the exceptions: `pack.toml` carries `format`
([manifest.md](manifest.md)), and `order.txt` has its own first line.

### 3.4 Floating-point values

The engine is deterministic and its parity gates compare bits, so a float read from a text file MUST come back with
the exact bits that were written:

- **Writers** MUST write a `f32` field with the shortest decimal digits that round-trip the value *as binary32*
  (at most 9 significant digits; Ryū, Adams 2018, https://github.com/ulfjack/ryu), and a `f64` field with the
  shortest digits that round-trip it as binary64 (at most 17), in this text form, which is always a TOML **float**:
  - plain notation with at least one digit after the `.` when x is ±0 or 1e-5 ≤ |x| < 1e16 (`1.0`, `-0.0`, `0.25`,
    `146.0`);
  - otherwise `d.ddde±x` with a one-digit mantissa before the point and at least one after it (`3.4028235e+38`,
    `7.006492e-46`).
  Rust's `{}` formatting is **not** this form (it prints `1`, `-0` and 39-digit integers, which TOML reads as
  integers or refuses), and `{:?}` is not guaranteed stable; writers use a formatter that implements the rule above
  and is tested against every binary32 value.
- **Readers.** A decimal literal in an `f32` field **means** round₃₂(round₆₄(x)): the literal is converted to
  binary64 with correct rounding (what TOML defines, https://toml.io/en/v1.0.0), then rounded to binary32. This is
  what TOML and JSON libraries in Python, JavaScript and Rust (`serde` with the `toml` crate) do, so any tool that
  reads the file as TOML gets the same bits, and no special reader hook is needed. Because the shortest binary32
  digits are written (above), the two readings agree for every binary32 value but two: an exhaustive test over all
  2³² values finds only ±7.038531e-26 (bits 0x15AE43FD), which read as 0x15AE43FE through binary64 (the
  double-rounding cases, Regan 2020,
  https://www.exploringbinary.com/double-rounding-errors-in-decimal-to-double-to-float-conversions/). Writers MUST
  therefore write those two values with the `bits:` escape (below). Hand-written decimals with more digits than
  binary32 holds simply mean what the rule above gives.
- **One parser.** The game, the launcher, Studio, the command-line tool and the web build read the format through
  one reader per syntax (one TOML reader, one JSON reader): two parsers that disagree on an
  input are a source of bugs no test of either alone finds (Momot et al., "The Seven Turrets of Babel", IEEE SecDev
  2016).
- `-0.0` keeps its sign. `inf`, `-inf` and `nan` are refused unless the field table allows them.
- **Floats are compared by their bits** wherever the format compares values (a field equal to its default or to the
  lower layer's value, guards, clashes, deduplication): `-0.0` and `0.0` differ, and two NaNs are equal only with
  equal bits. A writer that drops "unchanged" fields compares bits, so `-0.0` (which the original data uses, e.g.
  1,373 values in the menu packages) is never dropped as "equal to 0.0".
- **Exact bits escape**: any float field MAY instead be written as the string `"bits:0xXXXXXXXX"` (binary32) or
  `"bits:0xXXXXXXXXXXXXXXXX"` (binary64). Writers MUST use it for NaNs and for the two binary32 values
  ±7.038531e-26 (see Readers); writers MAY use it to keep a file byte-stable with a tool that cannot format floats
  as required.
- Quantised original values (an `i16` scaled by 1/8192) are stored as the exact real value when its decimal form
  is exact (every k/2ⁿ is) and the field table says so; otherwise the field stores the integer and names the scale.

### 3.5 JSON

Used only where an outside specification requires it (glTF 2.0) and for machine output (CLI `--json`). Same UTF-8
and float rules as 3.1 and 3.4 (JSON numbers are decimal literals; readers MUST parse `f32` targets directly to
binary32). Duplicate keys are refused.

### 3.6 Plain-text lists

`order.txt` ([layering.md](layering.md) §2) and `active.txt` ([layout.md](layout.md) §1) are the only line-based files. The first line names the file and its version;
`#` starts a comment line; blank lines are ignored.

## 4. Binary data

### 4.1 Scalars and byte order

All binary data defined by this format is **little-endian**. Types: `u8 i8 u16 i16 u32 i32 u64 i64` (two's
complement), `f32 f64` (IEEE 754 binary32/binary64, bits kept exactly, NaN payloads included), `bool8` (one byte,
0 or 1; a reader refuses other values). Outside formats keep their own rules (KTX 2.0 and glTF are also
little-endian).

### 4.2 Structs and layout tables

A binary struct is defined by a **layout table**, and a page defines every binary struct this way:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `count` | – | example row |

- Offsets are from the start of the struct, in hex. There is no implicit padding: every byte is a row. A row's class is
  one of: a **field** (stored; its default stated), **`pad`** (zero, checked), **`const`** (one value in all original
  data, not stored, checked at import; a different value sends the chunk to the residual or is refused as the page
  says) or ***derived*** (computed from other fields, not stored). Padding rows
  are named `pad` and MUST be written as zero; a reader MUST refuse non-zero padding (so a wrong layout is caught,
  not read as garbage).
- A struct's size and alignment are stated under its table. Arrays of structs have no gaps.
- `Type` may be another struct's name or `T[n]` for fixed arrays.

### 4.3 Alignment

Every array inside a binary file starts at a multiple of its element's alignment, and at least 16 bytes. Large
arrays the engine uses in place start at multiples of 64, the alignment of stored bundle entries
([layout.md](layout.md) §4), so they stay aligned in a mapped bundle; writers MAY use larger multiples (256, the
asset cache's section alignment for wgpu row copies, `src/cache/format.rs` `SECTION_ALIGN`).

### 4.4 External arrays (`.bin` files and array descriptors)

Large numeric data (collision faces, route points, sample data) does not belong in TOML. A kind MAY store it in a
binary file next to its TOML file and point at it with an **array descriptor**, the same idea as glTF's
buffer/bufferView/accessor (https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#buffers-and-buffer-views),
reduced to one level:

```toml
faces = { file = "1427.bin", offset = 0, count = 745, layout = "collision_face" }
```

| Key | Type | Required | Meaning |
|---|---|---|---|
| `file` | string | yes | A logical path relative to the TOML file's folder, no `..`; the file's extension is `.bin`. |
| `offset` | u64 | yes | Byte offset of the first element; a multiple of 16. |
| `count` | u64 | yes | Number of elements (0 allowed). |
| `layout` | string | yes | The name of a layout table on the kind's page; fixes the element size. |
| `digest` | digest | no | Digest of the array's bytes; writers SHOULD set it for base sets. |

A reader MUST refuse a descriptor whose range lies outside the file, overlaps another descriptor's range in the
same file with a different layout, or names an unknown layout. `.bin` files have no header of their own; they are
plain concatenations of arrays, each aligned as 4.3 says.

**Ranges inside one array.** A kind MAY address byte ranges *inside* one `raw_u8` array (a sample inside a sound
bank image, a movie's chunk inside its stream) with plain `offset` / `size` fields relative to that array's first
byte. Such ranges carry no alignment requirement (the original's own offsets are rarely aligned); the kind page
names the array they refer to, and a reader MUST refuse a range outside it.

### 4.5 Compression

Source files are stored uncompressed or as the outside format defines (KTX 2.0 supercompression, FLAC, PNG).
Bundles ([layout.md](layout.md) §4) decide whether an entry is deflated. The original's container compression
(JDLZ, HUFF) never appears in the format.

### 4.6 Choosing a representation

Every kind page chooses how original data is stored by these rules, in order:

1. **Exactness first.** The stored form MUST give the engine exactly the values it consumes today (to the bit). If
   an open format cannot hold them (decoded audio that the engine mixes as unrounded `f32`, say), the original data
   is kept as it is.
2. **Open where exact.** Otherwise the data is stored in an open, documented form: TOML for records and tables,
   glTF 2.0 for meshes, KTX 2.0 for GPU textures (original blocks unchanged), external arrays with layout tables for
   large numeric data.
3. **Codec passthrough when an open form is too large.** For **media bitstreams** only (audio, video, compressed
   animation curves): if the open lossless form would be more than **3×** the decoded container content
   ([residual.md](residual.md) §1), counting any shadow copy the residual must keep because no encoder can rebuild
   the original from it (§3.2 there), the original bitstream is kept in a documented framing (an open container
   where one fits, such as IVF for video) and decoded by Overbright's own decoder. The rule is applied **per kind**,
   over all its items: one kind has one representation. Records embedded inside such a media object (the
   skeletons inside an animation bank) stay in it; the kind page documents them and may expose them read-only.
   Records and tables otherwise always follow rule 2, whatever the size: their open forms are larger than the packed originals, and that is the price of being editable.

New content a pack or a community set adds is given in an open form (FLAC, PNG, KTX 2.0, glTF, TOML). A kind page
MAY also accept a passthrough form from packs where the community's existing tools produce it (engine recordings in
the GIN format, [audio.md](audio.md)) or where no open alternative is decided yet (a movie as VP6 video in IVF with its sound in the original's stream
format, until an open video codec is chosen, [movies.md](movies.md)).

## 5. Space, units and colours

### 5.1 Coordinate system

World space is the original's: **right-handed, +Z up**, one unit = one metre (the format docs treat it so, e.g.
the 32 m collision cells, docs/formats/cars-1.md). Car space: **+X forward, +Y left, +Z up**, origin as the original
puts it (src/replay_cams.rs, `CarState`). Every position, direction and bounding box in the format is in one of
these two spaces, and the field table says which.

glTF requires +Y up (glTF 2.0 §3.4). Meshes keep their original +Z-up coordinates in their buffers, bit for bit,
and the glTF scene's root node carries the fixed rotation that maps +Z up to +Y up, so outside viewers show them
upright ([meshes.md](meshes.md)).

### 5.2 Units in field names

When a number's unit is not obvious from the type, the field name ends in its unit:

| Suffix | Unit | Suffix | Unit |
|---|---|---|---|
| `_m` | metre | `_s` | second |
| `_mps` | metre per second | `_ticks` | world ticks of 1/60 s |
| `_deg` | degree | `_rad` | radian |
| `_bam16` | binary angle, 65,536 = one turn | `_hz` | hertz |
| `_db` | decibel | `_px` | pixel |
| `_kg` | kilogram | `_n` | newton |
| `_rpm` | revolutions per minute | `_pct` | percent (0–100) |
| `_frames` | scene and animation frames of 1/30 s | `_tonne` | tonne (1,000 kg) |
| `_kn` | kilonewton | `_knm` | kilonewton metre |
| `_ms` | millisecond | `_mph` | mile per hour (where the original stores it) |

A unit with no suffix here is named in the field table's Unit column. Values keep the original's unit where
converting would lose bits. A field converted for convenience (e.g. degrees
from binary angles) MUST be exact in both directions or it is not converted.

### 5.3 Rotations and transforms

Rotations are 3×3 matrices, row-major, each row one local axis expressed in the parent space, applied to row
vectors (`p' = p · M + t`), as the original does (docs/formats/world.md "Scenery"). Quaternions and Euler angles are
never canonical data; tools MAY show them.

### 5.4 Colours

`color` fields are `"#RRGGBBAA"`, sRGB-encoded, straight (not premultiplied) alpha. Binary vertex colours keep the
original's byte order (B, G, R, A — `D3DCOLOR`) and say so in their layout table (`color_bgra`). Pixel data keeps
whatever the texture format defines.

## 6. Names, keys, ids and references

### 6.1 Pack ids

`[a-z0-9][a-z0-9_-]{0,63}`. Unique among mounted packs. Base sets made by the importer use `nfsu2-<edition>`
([import.md](import.md) §3).

### 6.2 Logical paths

- `/`-separated components of `[A-Za-z0-9._-]`, each 1–100 characters, not `.` or `..`, not starting with `.`
  except the reserved `.studio/` folder; at most 240 bytes in all; no leading `/`.
- Case is significant in the logical tree, but a pack MUST NOT hold two paths that differ only in case (they would
  clash on Windows and macOS file systems). Readers refuse such a pack.
- Folder and file names chosen by this specification are lower case. Original game names inside paths keep their
  original case (`world/L4RA/…`, `cars/PEUGEOT206/…`).

### 6.3 Game names

Names the original uses (solids, textures, cars, parts) are kept exactly as the original stores them, including
case and punctuation (texture names contain `.`, `'`, `&` and `-`). Where the original stores a name truncated and
its hash is of the full name, the format stores **what the engine uses**:

- if the engine uses the truncated text (texture names, 23 characters), `name` holds that text and an optional
  `full_name` holds the completion when its hash proves it (tools only; [textures.md](textures.md));
- if the engine completes the name and uses the completion (solid names, 27 characters,
  `src/formats/hash.rs` `complete_truncated_name`), `name` holds the completion, and `name_truncated = true` marks a
  name no completion matched ([meshes.md](meshes.md)).

**Names that stand for original files** (a sound bank's file stem, a movie's file stem, an engine recording) are
looked up without regard to ASCII case, as the original's file opens are (its tables spell some names differently
from the install). The stored key keeps the install's spelling; a pack MUST NOT define two such keys of one kind
that differ only in case (`CONV.13`). Kind pages say which of their keys are of this sort.

**`nameref`**: a field that names an item by name where the name is known and by hash where it is not holds either
a `name` or a `hash32` string (`"XO_CONE_01"`, or its hash `"0x608A650A"`); readers tell them apart by the `0x` prefix (game
names never start with `0x`).

### 6.4 32-bit hash keys

The original names most things by `bin_hash(name)`: `h = 0xFFFFFFFF; for each byte c: h = h·33 + c (mod 2³²)`
(verified, docs/formats/world.md "Strings → hashes"; `src/formats/hash.rs`). In text the hash is written as
`"0x"` + 8 upper-case hex digits (`"0x3BE35F00"`). Where a kind keys items by hash, it also stores the name when the
name is known. Hashes are 32-bit, so new names can collide with existing ones: a writer MUST refuse a **new** name
whose hash equals the hash of a different name already in the layer stack for the same kind (the Studio's checks
run this; [layering.md](layering.md) §6).

### 6.5 Item keys

Each kind page defines its item key: a hash, a name, a number, or a tuple (e.g. a placement's region, scenery section
and index). Keys are unique within a kind in one pack. Overrides in higher layers address items by the same key
([layering.md](layering.md)).

**Order is data.** Where the original keeps items in an order (instances in a scenery section, records in a
table, entries in a bank), the importer MUST keep that order, and an item's position in it is part of its key when the original
has no unique name for it. The engine's behaviour can depend on the order (draw and cull order, first match wins,
random picks by index), and existing map mods address original placements through it: today by stream section, solid name and position
within 5 cm (docs/map-editor.md "File format"), and, in the map editor's next format-1 revision, with an additive
`nth` that tells exact duplicates apart by their rank in the original order ([world.md](world.md) §4.4). So,
normatively:

- Original scenery placements are keyed `(region, scenery section, index)`: the scenery section's own number
  (one stream section can hold several scenery sections; the original's links and visible lists name scenery
  sections), and `index` the placement's position in that scenery section's original instance array, counting
  from 0, skipped instances included ([world.md](world.md)). Two placements of the same solid at the same position
  (295 such pairs in L4RA's instance arrays) therefore have different keys.
- Items a mod pack **adds** are keyed by the pack — `(pack id, local id)`, or, for kinds named by names, a new name
  checked for hash collisions (§6.4; [layering.md](layering.md) §3.4) — never by an index into someone else's
  array, so adding never shifts another item's key.
- A reference to an original item MAY carry **guard** fields (for a placement: its solid and position). A reader
  checks them against the base set and refuses the reference on a mismatch (a pack made against another base set),
  instead of editing the wrong item.

### 6.6 References

A field that points at an item of another kind holds `"<kind>:<key>"`: `texture:0x3BE35F00`, `solid:XO_CONE_01`,
`car:PEUGOT`. The kind prefix is the kind's short name from [README.md](README.md) §3. References are resolved
against the whole layer stack after merging, never against one pack, so a pack can point at the base set's items
and at its dependencies' items. A reference that resolves to nothing is an error of the pack that holds it
(`CONV.9`), with one exception: where the original itself holds references to nothing and has a defined fallback
(a texture slot naming a texture no set holds draws `DEFAULTALPHA`, [textures.md](textures.md)), the base set keeps
those references as they are, and `CONV.9` applies only to references a mod pack adds or changes.

Tools (the CLI, MCP, Studio's history and problem lists) address items with the **`ob://` address**
`ob://<pack id>/<kind>/<key>[#<field path>][?guard=<value>]`, e.g.
`ob://docks-remix/placement/L4RA/1407/12#position`. It names the layer an edit lives in and, optionally, one field
and the value the tool expects there (so a tool can refuse to overwrite a value someone else changed). Addresses are
never stored inside data files; data files use the forms below.

Two forms exist in data, and each kind page says which a field takes. A third, **bare hashes**, is allowed only in
fields copied one to one from an original record layout (a scenery info's LOD hashes, a world animation's solid): the
field holds a `hash32` (`0x00000000` = none), and its page names the kind and the scope it resolves in, exactly as
for a scoped reference.

- **Item keys** name one item exactly, including where it lives (`solid:world/L4RA/1427.0/XO_CONE_01`,
  `placement:L4RA/1407/12`). Overrides and tools use them.
- **Scoped references** name an item the way the original's data does, by name or hash only
  (`texture:0x3BE35F00`, a material's texture slot, a scene's skeleton), and are resolved **in the referencing
  unit, in the engine's original lookup order** (the textures and meshes pages define that order: which sets are
  searched, in which order, first or last match). The same hash can name different items in different units; a
  scoped reference never means "any unit".

## 7. Digests

`"<alg>:<lower-case hex>"`, the grammar of OCI descriptors
(https://github.com/opencontainers/image-spec/blob/main/descriptor.md):

| Algorithm | Form | Used for |
|---|---|---|
| `sha256` | `sha256:` + 64 hex | Original files (the importer's verification, [import.md](import.md)); **everything compared between machines or against an untrusted party**: published `.obpk` files, mod-set pins, the pack identity a future online lobby compares. Collision resistance matters there. |
| `xxh3-128` | `xxh3-128:` + 32 hex | Local content addressing: source files, bundles, arrays, cache keys (fast; the asset cache already uses XXH3-128, `src/cache/keys.rs`). XXH3 is not a cryptographic hash and is never used where someone could craft a collision (https://github.com/Cyan4973/xxHash). The name matches the multicodec table's `xxh3-128` (https://github.com/multiformats/multicodec). |

- The 128-bit XXH3 value is written high 64 bits first.
- **Digests of lists** (a pack's content, a unit's inputs) hash a defined byte sequence, never a text dump: a domain
  tag (`ob-pack-content/1\0`, `ob-cook-inputs/1\0`), then per entry the logical path's UTF-8 bytes, a NUL, and the
  entry's raw digest bytes, entries sorted by path (as Git hashes trees,
  https://git-scm.com/book/en/v2/Git-Internals-Git-Objects).
- A **published reference** (a mod-set pin, a download) carries the `size` with the digest, and readers check the
  size before hashing (OCI does the same).
- The set of algorithms is part of each kind's schema version: adding one (BLAKE3, say) is a minor version, and a
  file using it declares that minor. Readers refuse algorithms their version does not list.

## 8. Unknown and opaque data

The format is lossless with respect to what the importer decodes ([parity.md](parity.md)). A kind keeps original
bytes it does not understand in one of three ways, chosen per field on its page:

1. **Named unknown field**: a field named `u_<offset>` (`u_0x1c`) with the original's type, documented as **open**.
   The offset is relative to the start of the struct the field sits in (for repeated records: the record, not the
   file). Where one table holds fields of two structs, the second struct's open fields are named
   `u_<struct>_<offset>` (`u_tree_0x06`). Writers MUST write it when its value differs from the field's documented default.
2. **Opaque bytes**: a `bytes` field (≤ 4096 bytes) or an external array with layout `raw_u8`.
3. **Residual**: whole chunks no decoder reads go to the base set's `residual/` folder ([residual.md](residual.md)).

A reader MUST carry fields 1 and 2 through every load, merge and save unchanged.

## 9. Extensions and extras

- **`[extras]`**: any TOML file MAY have one top-level `[extras]` table (and an item MAY have an `extras` sub-table)
  with free content for tools (an editor's notes, a source file path). Readers MUST ignore it for behaviour and
  MUST keep it when re-writing the file. Its size counts toward the file's limit.
- **Extensions**: a new feature that is not (yet) part of a kind's schema lives in `[ext.<NAME>]` tables, where
  `<NAME>` is `[A-Z0-9]+_[a-z0-9_]+` and the prefix names who defines it (`OB_` is reserved for Overbright itself),
  as glTF does (https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#specifying-extensions). A pack lists
  every extension it uses in `pack.toml` `extensions_used` and those a reader must understand in
  `extensions_required` ([manifest.md](manifest.md)). An extension is **required** when ignoring it would change
  play, sound or pictures; it is merely *used* when ignoring it changes only what tools show. Rules:
  - a reader MUST refuse a pack that requires an extension it does not know (`CONV.14`);
  - it MUST ignore, with one warning per pack, `[ext.*]` tables of unknown extensions that are only used, and MUST
    keep them unchanged when re-writing the file (as it keeps `[extras]`);
  - an `[ext.X]` table whose `X` is not listed in `extensions_used` is refused (`CONV.16`), and
    `extensions_required` MUST be a subset of `extensions_used` (`MAN.9`);
  - the registry of `OB_` extensions ([README.md](README.md) §3) marks those that are always required; listing one
    only as used is refused (`CONV.16`);
  - each extension is declared **safe to copy** or not, in the pack's `extensions_safe_to_copy`
    ([manifest.md](manifest.md) §3); an extension not listed there is not. A tool that rewrites an item (Studio, the CLI) keeps
    unknown safe-to-copy extension data and refuses to edit an item carrying unknown data that is not safe to copy,
    since it cannot keep that data consistent (the model of PNG's chunk property bits, https://www.w3.org/TR/png-3/
    §5.4, and of EA IFF 85's "do not edit a file you don't know how to create"). The game, which only reads, ignores
    unknown used extensions either way.
- `[extras]` MUST NOT hold data derived from the item (a cached bounding box, a thumbnail of it): a tool that edits
  the item cannot know to update it, so it would go stale.
- glTF files use glTF's own extension mechanism with `OB_` names ([meshes.md](meshes.md)).

## 10. Limits

Each kind page states its limits (file size, items per file, string lengths). Defaults when a page says nothing:

| What | Default limit |
|---|---|
| A TOML file | 16 MiB |
| A `.bin` file | 1 GiB |
| Items per TOML file | 1,000,000 |
| A `string` field | 200 characters |
| Array nesting | 4 levels |

A reader MUST check a limit before allocating for it (the parsers already follow this rule,
`src/formats/reader.rs` `Limits`).

## 11. Errors

- Every refusal names: the pack id, the logical path, the line (TOML) or byte offset (binary) when known, the field,
  and a **rule id**. Rule ids are `<PAGE>-<n>` (`CONV.3`, `TEX.12`), defined on the pages next to the rule, and
  never reused for another rule. Code and docs share them ([versioning.md](versioning.md) §6 keeps them in step).
- **A pack is all or nothing.** If any file of a mod pack is refused, the pack is not mounted and the launcher shows
  the first reason; nothing of it is played. A partly loaded pack is worse than a missing one. This holds for both
  classes of error:
  - **errors in the pack itself** (syntax, schema, limits, a broken bundle);
  - **errors against the stack** — a reference that resolves to nothing (`CONV.9`), a `merge` of an item no lower
    layer has (`MRG.1`), a guard that does not match the base set (`WLD.11`), a dependency missing (`ORD.2`). These
    mean the pack was made against another base set or another version of a dependency; one stale item refuses the
    whole pack, and the launcher's message names how many items are stale and the first of them ("made for another
    game version: 3 placements not found, first L4RA/1407/12").
  Studio opens such a pack anyway and lists every stale item for repair (re-target, drop, keep for later); only
  mounting it for play is refused. **Legacy packs** (today's format-1 map mods and texture packs, [layering.md](layering.md)
  §7) keep today's rule instead: an edit that resolves to nothing is skipped and counted in the log (`WLD.15`), so
  existing mods behave as they do now.
- **The base set** is checked per bundle ([layout.md](layout.md) §5). A refused bundle makes the base set
  *damaged*: the game does not start on it, and the launcher offers repair ([import.md](import.md) §8).
- Warnings (unknown "used" extensions, an `extras` table over a soft size) never change what is played.

Checks run in this order, so a file from a newer Overbright reports what it needs rather than a symptom: the file's
`schema` (`CONV.3`, `VER.1`, `VER.2`), then fields newer than the declared version (`VER.6`), then unknown keys
(`CONV.4`), then the field rules.

Common rules:

| Rule | Text |
|---|---|
| CONV.1 | Not UTF-8, has a BOM or a NUL. |
| CONV.2 | TOML syntax error (with the parser's line and column). |
| CONV.3 | Missing `schema`, or a kind this reader does not know (versions: `VER.1`, `VER.2`). |
| CONV.4 | Unknown key outside `[extras]` / `[ext.*]`. |
| CONV.5 | Missing required field. |
| CONV.6 | Value of the wrong type, out of range, or not in the enum. |
| CONV.7 | Float that is NaN or infinite where not allowed, or a malformed `bits:` escape. |
| CONV.8 | Bad name, hash, digest or reference syntax. |
| CONV.9 | Reference that resolves to nothing in the layer stack. |
| CONV.10 | Array descriptor out of range, misaligned, overlapping, or of an unknown layout. |
| CONV.11 | Non-zero padding in a binary struct. |
| CONV.12 | A limit exceeded. |
| CONV.13 | Two logical paths differing only in case; a path that breaks 6.2; two keys of a file-name kind (§6.3) differing only in case within one layer stack. |
| CONV.14 | A required extension this reader does not know. |
| CONV.15 | A new name whose hash collides with another name of the same kind in the stack. |
| CONV.16 | An `[ext.X]` table not declared in `extensions_used`, or an always-required `OB_` extension declared only as used. |

## 12. Page template for kinds

Every kind page has these sections, in this order, so readers and tools find things in the same place:

1. **Scope** — what original data maps here (files, chunk ids), and what does not.
2. **Files and keys** — folders, file names, item keys, an example tree.
3. **Source form** — one field table per file kind (Field, Type, Unit, Required, Default, Meaning), limits,
   an example file. Binary files: layout tables.
4. **Layering** — what a mod pack may add, override, disable; the clash key; list merge rules.
5. **Import** — how the importer fills it from the original (decoder, chunk ids, field mapping), and what goes to
   the residual.
6. **Equality and verification** — the equality level, how the import proves it, the tests.
7. **Cooked form** — what the cache keeps for this kind and its cook unit ([cooked.md](cooked.md)).
8. **Errors** — the kind's rule ids.
9. **Versioning and extension** — what a minor version may add; known extension points.
10. **Open points** — inferred and open fields, carried as section 8 says.
11. **Sizes and timing** — reference install numbers; guesses marked.

Schemas: each TOML file kind has a JSON Schema (draft 2020-12, https://json-schema.org/draft/2020-12) in
`schemas/`, applied to the TOML document after the standard TOML→JSON mapping (tables → objects, arrays → arrays).
Shared definitions are in `schemas/common.schema.json`. A schema checks shape, not cross-file rules; the page's
rule table is the full set of checks. Where a schema already refuses something that a page rule names (an
operation the kind does not allow, refused by the schema's enum), a reader reports the **page's** rule id (`MRG.3`
here), not the generic shape rule (`CONV.6`): the rule id says why, the schema only says where.
